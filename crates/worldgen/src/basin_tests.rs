use super::*;
use engine_core::VoxelScalePreset;

fn planet(generator_version: u32) -> WorldSampleConfig {
    WorldSampleConfig { planet: None, generator_version, seed: 74, world_type: WorldType::Planet, voxel_scale: VoxelScalePreset::OneMeter }
}

#[test]
fn older_planets_keep_the_noise_shell() {
    let old = planet(VERSION - 1);
    assert!(!covers(&old, 0, 0));
    assert!(column(&old, 0, 0, || unreachable!()).is_none());
    assert!(!crate::geo::active(&old));
}

#[test]
fn solved_face_has_drainage_relief_and_rivers() {
    let cfg = planet(VERSION);
    assert!(crate::geo::active(&cfg));
    let (mut low, mut high, mut land, mut wet) = (i32::MAX, i32::MIN, 0, 0);
    for gz in -24..=24 {
        for gx in -24..=24 {
            let (x, z) = (gx * 600, gz * 600);
            let c = crate::sample_column(&cfg, x, z);
            low = low.min(c.height);
            high = high.max(c.height);
            if c.height > cfg.water_level() {
                land += 1;
            }
            if crate::geo::ground(&cfg, x as f32, z as f32, 1.0).water_m.is_some_and(|w| w > crate::geo::SEA_M as f32 + 1.0) {
                wet += 1;
            }
        }
    }
    assert!(high - low > 200, "relief {low}..{high}");
    assert!(land > 600, "only {land} of 2401 samples are land");
    assert!(wet > 0, "no inland river or lake on the face");
    let springs = crate::geo::river_sources(&cfg, glam::Vec2::ZERO, 16_000.0);
    assert!(springs.len() > 20, "{} river sources", springs.len());
}

#[test]
fn face_edge_meets_the_shell_without_a_wall() {
    let cfg = planet(VERSION);
    let half = atlas::HALF_M as i32;
    for along in [-9_000, -2_500, 0, 4_000, 11_000] {
        for (inside, outside) in [((half - 2, along), (half + 2, along)), ((along, -half + 2), (along, -half - 2))] {
            let a = crate::sample_column(&cfg, inside.0, inside.1).height;
            let b = crate::sample_column(&cfg, outside.0, outside.1).height;
            assert!((a - b).abs() <= 3, "step {a} -> {b} at {inside:?}");
        }
    }
}

/// Prints river, forest and ridge columns near spawn for review captures.
#[test]
#[ignore]
fn survey_review_poses() {
    let cfg = planet(VERSION);
    let (mut river, mut forest, mut ridge) = (None, None, (i32::MIN, 0, 0));
    for r in (0..6_000).step_by(48) {
        for k in 0..16 {
            let a = k as f32 * std::f32::consts::TAU / 16.0;
            let (x, z) = ((a.cos() * r as f32) as i32, (a.sin() * r as f32) as i32);
            let c = crate::sample_column(&cfg, x, z);
            let g = crate::geo::ground(&cfg, x as f32, z as f32, 1.0);
            if river.is_none() && c.height > cfg.water_level() + 8 && g.water_m.is_some() {
                river = Some((x, z, c.height));
            }
            if forest.is_none() && c.vegetation_density > 0.6 && c.height > cfg.water_level() + 6 {
                forest = Some((x, z, c.height, c.surface));
            }
            if c.height > ridge.0 {
                ridge = (c.height, x, z);
            }
        }
    }
    eprintln!("spawn {:?}", crate::sample_column(&cfg, 0, 0).height);
    eprintln!("river {river:?}\nforest {forest:?}\nridge {ridge:?}");
}

#[test]
#[ignore]
fn time_face_cover_samples() {
    let cfg = planet(VERSION);
    let _ = crate::sample_column(&cfg, 0, 0);
    let t = std::time::Instant::now();
    for i in 0..20_000 {
        let _ = cover_column(&cfg, (i % 141) * 211 - 14_800, (i / 141) * 207 - 14_600, 83.0);
    }
    eprintln!("cover: {:.2} us/sample", t.elapsed().as_secs_f64() * 50.0);
}

#[test]
fn placer_gold_is_rare_and_fed_from_belts_upstream() {
    let cfg = planet(VERSION);
    assert!(crate::placer::grade(&planet(VERSION - 1), 0.0, 0.0).is_none());
    let (mut land, mut paying, mut rich_off_belt) = (0, 0, 0);
    for gz in -60..=60 {
        for gx in -60..=60 {
            let (x, z) = (gx as f32 * 230.0, gz as f32 * 230.0);
            let Some(g) = crate::placer::grade(&cfg, x, z) else { continue };
            if crate::geo::ground(&cfg, x, z, 64.0).top_m <= crate::geo::SEA_M as f32 {
                continue;
            }
            land += 1;
            if g > crate::placer::PAYDIRT_GRADE {
                paying += 1;
                // Gold reached here from a belt: none sits more than a few km from one.
                let near_belt = (-12..=12).any(|dz| (-12..=12).any(|dx| {
                    crate::placer::lode(&cfg, x + dx as f32 * 400.0, z + dz as f32 * 400.0) > 0.0
                }));
                if !near_belt {
                    rich_off_belt += 1;
                }
            }
        }
    }
    assert!(paying > 0, "no paying ground in {land} land samples");
    assert!(paying * 20 < land, "{paying} of {land} land samples pay");
    assert_eq!(rich_off_belt, 0, "rich ground with no belt upstream");
}

#[test]
fn far_cover_follows_the_solved_height() {
    let cfg = planet(VERSION);
    for (x, z) in [(0, 0), (-5011, -2075), (3000, 7000), (-9000, 4000)] {
        let far = cover_column(&cfg, x, z, 83.0).map(|c| c.height).unwrap_or_default();
        let near = crate::sample_column(&cfg, x, z).height;
        assert!((far - near).abs() < 40, "{x},{z}: cover {far} vs column {near}");
    }
    assert!(cover_column(&planet(VERSION - 1), 0, 0, 83.0).is_none());
}

/// Prints a land map of atlas faces 0 and 1 for a version.
#[test]
#[ignore]
fn print_face_map() {
    let v: u32 = std::env::var("MAP_VERSION").ok().and_then(|s| s.parse().ok()).unwrap_or(VERSION);
    let cfg = planet(v);
    for face in 0..2 {
        eprintln!("v{v} face {face}");
        for gz in -12..=12 {
            let row: String = (-24..=24).map(|gx| {
                let x = face * atlas::PITCH_M as i32 + gx * 660;
                let c = crate::sample_column(&cfg, x, gz * 1300);
                if c.height <= cfg.water_level() { '.' } else if c.height > cfg.water_level() + 150 { '^' } else { '#' }
            }).collect();
            eprintln!("{row}");
        }
    }
}

#[test]
fn planet_labels_follow_the_planet_ground() {
    let cfg = planet(VERSION);
    assert_eq!(planet_label(&cfg, 0, 0), None, "the solved face uses the biome map");
    let hills = WorldSampleConfig { world_type: WorldType::Hills, ..cfg };
    assert_eq!(planet_label(&hills, 40_000, 0), None);
    let mut seen = Vec::new();
    for older in [planet(VERSION - 1)] {
        for i in 0..400 {
            let (x, z) = ((i % 20) * 1500 - 15_000, (i / 20) * 1500 - 15_000);
            let label = planet_label(&older, x, z).expect("older planets always label their ground");
            if crate::sample_column(&older, x, z).height < older.water_level() {
                assert_eq!(label, "Ocean");
            }
            if !seen.contains(&label) { seen.push(label); }
        }
    }
    assert!(seen.len() >= 3, "{seen:?}");
}

#[test]
fn quartz_veins_stay_inside_lode_belts() {
    let cfg = planet(VERSION);
    let (mut veins, mut in_belt) = (0, 0);
    for i in 0..40_000 {
        let m = glam::Vec3::new((i % 200) as f32 * 37.0 - 7_000.0, 20.0 + (i / 8_000) as f32 * 9.0, ((i / 200) % 40) as f32 * 211.0 - 4_000.0);
        if crate::placer::lode(&cfg, m.x, m.z) >= 0.45 {
            in_belt += 1;
        }
        if crate::placer::vein(&cfg, m) {
            veins += 1;
            assert!(crate::placer::lode(&cfg, m.x, m.z) >= 0.45);
        }
    }
    assert!(veins > 0, "no veins in {in_belt} belt samples");
    assert!(veins * 8 < in_belt, "{veins} veins in {in_belt} belt samples");
}

/// Prints the nearest bare-rock vein outcrop and the richest paydirt reach to spawn.
#[test]
#[ignore]
fn survey_gold_poses() {
    let cfg = planet(VERSION);
    let (mut outcrop, mut paydirt) = (None, None::<(f32, i32, i32)>);
    for r in (0..9_000).step_by(16) {
        for k in 0..64 {
            let a = k as f32 * std::f32::consts::TAU / 64.0;
            let (x, z) = ((a.cos() * r as f32) as i32, (a.sin() * r as f32) as i32);
            let c = crate::sample_column(&cfg, x, z);
            if outcrop.is_none() && matches!(c.surface, crate::BlockId::Stone | crate::BlockId::Gravel)
                && (0..3).any(|d| crate::sample_block(&cfg, glam::IVec3::new(x, c.height - d, z)) == crate::BlockId::GoldQuartz)
            {
                outcrop = Some((x, z, c.height));
            }
            if let Some(g) = crate::placer::grade(&cfg, x as f32, z as f32)
                && g > paydirt.map_or(crate::placer::PAYDIRT_GRADE, |p| p.0)
                && crate::sample_block(&cfg, glam::IVec3::new(x, c.height, z)) == crate::BlockId::Paydirt
            {
                paydirt = Some((g, x, z));
            }
        }
        if outcrop.is_some() && paydirt.is_some() && r > 4_000 { break; }
    }
    eprintln!("outcrop {outcrop:?}\npaydirt {paydirt:?}");
}

#[test]
#[ignore]
fn print_cover_error_near_spawn() {
    let cfg = planet(VERSION);
    let mut diffs = Vec::new();
    for i in 0..400 {
        let (x, z) = ((i % 20) * 24 - 240, (i / 20) * 24 - 240);
        let far = cover_column(&cfg, x, z, 40.0).unwrap().height;
        let near = crate::sample_column(&cfg, x, z).height;
        diffs.push(far - near);
    }
    diffs.sort();
    let mean = diffs.iter().sum::<i32>() as f32 / diffs.len() as f32;
    eprintln!("cover - exact: min {} p10 {} median {} p90 {} max {} mean {mean:.2}", diffs[0], diffs[40], diffs[200], diffs[360], diffs[399]);
}

#[test]
fn far_cover_shows_the_walked_surface() {
    // Far cover once used its own biome table and agreed on half the land.
    let cfg = planet(VERSION);
    let (mut land, mut same) = (0, 0);
    for gz in -4..=4 {
        for gx in -4..=4 {
            let (x, z) = (gx * 3400 + 13, gz * 3400 + 7);
            let exact = crate::sample_column(&cfg, x, z);
            if exact.height < cfg.water_level() { continue; }
            let cover = cover_column(&cfg, x, z, 250.0).expect("inside the solved face");
            land += 1;
            same += usize::from(cover.surface == exact.surface);
        }
    }
    assert!(land > 25, "{land} land samples");
    assert!(same * 100 >= land * 85, "{same} of {land} far samples match");
}
