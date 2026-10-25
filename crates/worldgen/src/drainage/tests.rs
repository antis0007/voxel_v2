use super::*;
#[test]
fn environment_drainage_shared_connections_descend_across_negative_regions() {
    let cfg = WorldSampleConfig {
        generator_version: ENVIRONMENT_GENERATOR,
        seed: 173,
        world_type: WorldType::Hills,
        voxel_scale: VoxelScalePreset::OneMeter,
    };
    for z in -4..=4 {
        for x in -4..=4 {
            if let Outlet::Downhill(a, b) = outlet(&cfg, x, z) {
                assert!(node(&cfg, a, b).head_m < node(&cfg, x, z).head_m);
                let fine = WorldSampleConfig { voxel_scale: VoxelScalePreset::QuarterMeter, ..cfg };
                assert_eq!(outlet(&cfg, x, z), outlet(&fine, x, z));
                assert_eq!(node(&cfg, x, z), node(&fine, x, z));
            }
        }
    }
}
#[test]
fn environment_drainage_is_disabled_for_historical_terrain() {
    let cfg = WorldSampleConfig {
        generator_version: ENVIRONMENT_GENERATOR - 1,
        seed: 173,
        world_type: WorldType::Hills,
        voxel_scale: VoxelScalePreset::OneMeter,
    };
    assert_eq!(sample(&cfg, -320.0, 700.0).bank, 0.0);
}

#[test]
fn environment_drainage_places_water_in_ordinary_generated_channels() {
    let cfg = WorldSampleConfig {
        generator_version: ENVIRONMENT_GENERATOR,
        seed: 2738958700,
        world_type: WorldType::Hills,
        voxel_scale: VoxelScalePreset::OneMeter,
    };
    let mut count = 0;
    for z in -6..=6 {
        for x in -6..=6 {
            let source = node(&cfg, x, z);
            let Outlet::Downhill(a, b) = outlet(&cfg, x, z) else { continue };
            let target = node(&cfg, a, b);
            if source.rainfall < 0.35 || target.head_m < 20.0 {
                continue;
            }
            let p = (source.center_m + target.center_m) * 0.5;
            let Some(head) = sample(&cfg, p.x, p.y).water_m else {
                continue;
            };
            let voxel = glam::IVec3::new(p.x.round() as i32, head.ceil() as i32 - 1, p.y.round() as i32);
            if super::super::sample_block(&cfg, voxel) == super::super::BlockId::Water {
                count += 1;
                if count == 1 {
                    println!(
                        "environment generated channel seed={} x={} z={} head_m={head:.2}",
                        cfg.seed, voxel.x, voxel.z
                    );
                }
            }
        }
    }
    assert!(count >= 3, "only {count} generated wet channel sections");
}

fn meander_cfg(seed: u32) -> WorldSampleConfig {
    WorldSampleConfig {
        generator_version: MEANDER_GENERATOR,
        seed,
        world_type: WorldType::Hills,
        voxel_scale: VoxelScalePreset::OneMeter,
    }
}

#[test]
fn meander_drainage_descends_flows_diagonally_and_ignores_voxel_size() {
    let cfg = meander_cfg(2738958700);
    let fine = WorldSampleConfig { voxel_scale: VoxelScalePreset::QuarterMeter, ..cfg };
    let (mut axis, mut diagonal) = (0, 0);
    for z in -8..=8 {
        for x in -8..=8 {
            assert_eq!(outlet(&cfg, x, z), outlet(&fine, x, z));
            if let Outlet::Downhill(a, b) = outlet(&cfg, x, z) {
                assert!(node(&cfg, a, b).head_m < node(&cfg, x, z).head_m);
                assert!((a - x).abs() <= 1 && (b - z).abs() <= 1);
                if a != x && b != z { diagonal += 1 } else { axis += 1 }
            }
        }
    }
    assert!(diagonal * 5 >= axis, "network still axis-bound: {diagonal} diagonal vs {axis} axis edges");
}

#[test]
fn meander_channels_stay_inside_the_sample_window() {
    // A wider node search must find nothing the 4x4 window misses,
    // including across negative coordinates.
    let cfg = meander_cfg(173);
    for z in (-900..900).step_by(7) {
        for x in (-900..900).step_by(11) {
            let (x, z) = (x as f32 + 0.5, z as f32 + 0.5);
            let near = sample(&cfg, x, z);
            let wide = sample_window(&cfg, x, z, -3..=4);
            assert_eq!((near.bank, near.bed_m, near.water_m), (wide.bank, wide.bed_m, wide.water_m), "at {x},{z}");
        }
    }
}

#[test]
fn meander_banks_have_no_seams() {
    // Bank falls off over 8 m, so a 0.25 m step changes it by at most
    // ~0.05; a seam at a reach end, cap or the early-out shows as a jump.
    let cfg = meander_cfg(2738958700);
    for row in -40..40 {
        let z = row as f32 * 23.0 + 0.37;
        let mut last = sample(&cfg, -1000.0, z).bank;
        for i in 1..8_000 {
            let x = -1000.0 + i as f32 * 0.25;
            let bank = sample(&cfg, x, z).bank;
            assert!((bank - last).abs() < 0.12, "bank jumps {last} -> {bank} at {x},{z}");
            last = bank;
        }
    }
}

#[test]
fn meander_channels_bend_and_reach_their_outlet() {
    let cfg = meander_cfg(2738958700);
    let (mut bent, mut edges) = (0, 0);
    for z in -6..=6 {
        for x in -6..=6 {
            let source = node(&cfg, x, z);
            let Outlet::Downhill(a, b) = outlet(&cfg, x, z) else { continue };
            let target = node(&cfg, a, b);
            edges += 1;
            // The channel touches both nodes, so tributaries join their outlet.
            for end in [source.center_m, target.center_m] {
                assert!(meander(&cfg, (x, z), source, target, end).0 < 0.01);
            }
            // The chord midpoint is usually off the bent centreline.
            let mid = (source.center_m + target.center_m) * 0.5;
            if meander(&cfg, (x, z), source, target, mid).0 > 8.0 {
                bent += 1;
            }
        }
    }
    assert!(edges > 30 && bent * 2 > edges, "{bent} of {edges} channels bend");
}

#[test]
fn meander_drainage_places_water_and_leaves_v49_untouched() {
    let cfg = meander_cfg(2738958700);
    let wet = (-1500..1500)
        .step_by(6)
        .flat_map(|z| (-1500..1500).step_by(6).map(move |x| (x as f32, z as f32)))
        .filter(|&(x, z)| sample(&cfg, x, z).water_m.is_some())
        .count();
    assert!(wet > 200, "only {wet} wet samples");
    let review = (-6..=6)
        .flat_map(|z| (-6..=6).map(move |x| (x, z)))
        .filter_map(|(x, z)| match outlet(&cfg, x, z) {
            Outlet::Downhill(a, b) if node(&cfg, x, z).rainfall > 0.3 => {
                Some((node(&cfg, x, z).center_m + node(&cfg, a, b).center_m) * 0.5)
            }
            _ => None,
        })
        .min_by(|p, q| p.length().total_cmp(&q.length()));
    println!("meander review reach seed={} midpoint={review:?}", cfg.seed);
    let old = WorldSampleConfig { generator_version: MEANDER_GENERATOR - 1, ..cfg };
    let lattice = |x: i32, z: i32| node(&old, x, z).center_m - Vec2::new(x as f32, z as f32) * SPACING;
    assert!((-3..=3).all(|i| lattice(i, -i).abs().max_element() <= 32.0));
    assert!((-4..=4).all(|i| matches!(outlet(&old, i, i), Outlet::Downhill(a, b) if (a - i).abs() + (b - i).abs() == 1) || !matches!(outlet(&old, i, i), Outlet::Downhill(..))));
}
