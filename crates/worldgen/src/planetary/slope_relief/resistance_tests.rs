//! Actual Stage outputs against a frozen pre-reuse Stage, plus fixed-limit proof.
use super::*;
#[path = "resistance_reference.rs"]
mod reference;

#[derive(Default)]
struct StageAudit {
    fixed: usize,
    active: usize,
    historical_some: usize,
    historical_none: usize,
    analytic: bool,
}

fn same_stage(a: &Stage, b: &Stage) {
    let Stage { grid: ag, rock_delta: ar, soil: asoil, base: ab } = a;
    let Stage { grid: bg, rock_delta: br, soil: bsoil, base: bb } = b;
    assert_eq!(ag.n, bg.n);
    assert_eq!(ag.cell_m.to_bits(), bg.cell_m.to_bits());
    assert_eq!(ag.min_m.map(f64::to_bits), bg.min_m.map(f64::to_bits));
    let bits = |v: &[f32]| v.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(ar), bits(br), "rock_delta");
    assert_eq!(bits(asoil), bits(bsoil), "soil");
    match (ab, bb) {
        (Some(a), Some(b)) => assert_eq!(bits(a), bits(b), "base"),
        (None, None) => {}
        _ => panic!("base presence differs"),
    }
}

#[test]
fn fixed_talus_limits_are_not_read_as_sources_or_neighbours() {
    let grid = Grid { n: 7, cell_m: 8.0, min_m: [0.0, 0.0] };
    for aligned in [false, true] {
        let mesh = Mesh::with_alignment(grid, aligned);
        let fixed: Vec<_> = (0..grid.len()).map(|i| grid.on_border(i) || i == 24).collect();
        assert!(mesh.links.iter().enumerate().any(|(i, links)|
            !fixed[i] && links.as_slice().iter().any(|e| fixed[e.to as usize])));
        let rock: Vec<_> = (0..grid.len()).map(|i| 160.0 - (i % 7) as f64 * 7.0
            - (i / 7) as f64 * 5.0 + (i % 3) as f64 * 3.0).collect();
        let soil: Vec<_> = (0..grid.len()).map(|i| (i % 5) as f64 * 0.17).collect();
        let mut expected_rock = rock.clone();
        let mut expected_soil = soil.clone();
        let mut expected_height = vec![0.0; grid.len()];
        let mut expected_mass = vec![0.0; grid.len()];
        let limits = vec![0.6; grid.len()];
        move_talus(&mesh, &fixed, &limits, &mut expected_rock, &mut expected_soil,
            &mut expected_height, &mut expected_mass);
        for fixed_value in [0.0, -0.0, -1000.0, f64::NEG_INFINITY, f64::NAN] {
            let limits: Vec<_> = fixed.iter().map(|&f| if f { fixed_value } else { 0.6 }).collect();
            let mut actual_rock = rock.clone();
            let mut actual_soil = soil.clone();
            let mut actual_height = vec![0.0; grid.len()];
            let mut actual_mass = vec![0.0; grid.len()];
            move_talus(&mesh, &fixed, &limits, &mut actual_rock, &mut actual_soil,
                &mut actual_height, &mut actual_mass);
            for (actual, expected) in [(&actual_rock, &expected_rock), (&actual_soil, &expected_soil),
                (&actual_height, &expected_height), (&actual_mass, &expected_mass)] {
                assert_eq!(actual.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                    expected.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
            }
        }
    }
}

fn config() -> WorldSampleConfig {
    if let Some(path) = std::env::var_os("AXIOM_FINE_CACHE") {
        assert!(crate::planetary::configure_geology_cache(path.into()));
    }
    let generator_version = std::env::var("AXIOM_FINE_VERSION")
        .unwrap_or_else(|_| "287".into()).parse().unwrap();
    assert!([275, 277, 278, 279, 280, 283, 286, 287].contains(&generator_version));
    WorldSampleConfig { generator_version,
        seed: std::env::var("AXIOM_FINE_SEED").unwrap_or_else(|_| "74".into()).parse().unwrap(),
        world_type: crate::WorldType::Planet, planet: None,
        voxel_scale: engine_core::VoxelScalePreset::OneMeter }
}

fn exposed_key(world: &World, cfg: &WorldSampleConfig) -> Key {
    for &dir in &world.mesh.directions {
        let volcanic = world.volcano_shape(dir);
        if volcanic.active_caldera || world.edifice_lift(dir) > 12.0 { continue; }
        let ground = world.ground_base(cfg, dir, CELL_M as f32, volcanic);
        if exposure(&ground, crate::planetary::watershed_erosion(cfg.generator_version),
            crate::planetary::rifted_margins(cfg.generator_version) || crate::planetary::connected_relief(cfg.generator_version),
            cfg.generator_version == crate::planetary::LAYERED_CANYON_GENERATOR,
            crate::planetary::evolved_watersheds(cfg.generator_version)) < 0.1 { continue; }
        let (face, s, t) = Projection::Tangent.from_sphere(dir);
        if s.abs().max(t.abs()) > 0.85 { continue; }
        return Key { face: face as u8, x: (s * atlas::HALF_M / CORE_M).floor() as i32,
            z: (t * atlas::HALF_M / CORE_M).floor() as i32 };
    }
    panic!("fixture found no exposed interior tile");
}

fn compare_chain(world: &World, cfg: &WorldSampleConfig, key: Key, missing_history: bool) {
    let mut actual_parent = None;
    let mut expected_parent = None;
    let mut saw_active = false;
    for (spacing, passes) in [(32.0, 8), (16.0, 7), (8.0, 8), (4.0, 5)] {
        let mut audit = StageAudit::default();
        let expected = Stage::solve_reference(world, cfg, key, spacing, passes,
            expected_parent.as_ref(), false, &mut audit);
        let actual = Stage::solve(world, cfg, key, spacing, passes, actual_parent.as_ref(), false);
        same_stage(&actual, &expected);
        assert!(audit.fixed > 0, "border fixture coverage");
        saw_active |= audit.active > 0;
        if crate::planetary::geological_history(cfg.generator_version) {
            if missing_history {
                assert_eq!(audit.historical_some, 0);
                assert_eq!(audit.historical_none, actual.grid.len());
            } else {
                assert_eq!(audit.historical_some, actual.grid.len());
                assert_eq!(audit.historical_none, 0);
            }
        }
        assert_eq!(audit.analytic, cfg.generator_version == crate::planetary::LAYERED_CANYON_GENERATOR);
        eprintln!("[fine-resistance-exact] version={} seed={} spacing={spacing} passes={passes} fixed={} active={} historical_some={} historical_none={}",
            cfg.generator_version, cfg.seed, audit.fixed, audit.active, audit.historical_some, audit.historical_none);
        actual_parent = Some(actual);
        expected_parent = Some(expected);
    }
    assert!(saw_active, "fixture must exercise non-fixed resistance queries");
}

#[test]
#[ignore = "actual-world Stage comparison; isolated process per AXIOM_FINE_VERSION/SEED"]
fn fine_resistance_reuse_matches_frozen_stage_with_parents_and_missing_history() {
    let cfg = config();
    let progress = geo::Progress::default();
    let mut world = crate::planetary::build(&cfg, &progress, true);
    let key = exposed_key(&world, &cfg);
    compare_chain(&world, &cfg, key, false);
    if crate::planetary::geological_history(cfg.generator_version) {
        // Exercise Some(history vector) whose per-node RockColumns are None.
        // This must remain resistance 1.0, not an analytic-strata fallback.
        world.history_top.clear();
        world.slope_tiles = Tiles::default();
        compare_chain(&world, &cfg, key, true);
    }
}
