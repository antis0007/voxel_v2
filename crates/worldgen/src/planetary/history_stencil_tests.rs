//! Frozen pre-reuse formula, full column bits and unchanged memo semantics.
use super::*;

impl World {
    fn history_strata_reference(&self, cfg: &WorldSampleConfig, dir: DVec3) -> Option<drift::history::RockColumn> {
        if self.history_top.is_empty() { return None; }
        let key = ((cfg.seed, cfg.planet_fingerprint(), cfg.generator_version), dir.to_array().map(f64::to_bits));
        if let Some(column) = HISTORY_ROCK.with(|cache| cache.borrow().as_ref().filter(|(k, _)| *k == key).map(|(_, c)| *c)) {
            return Some(column);
        }
        let (mut beds, weathered_myr) = self.plates.history_column(dir)?;
        let raw_top = self.mesh.sample(&self.history_top, dir);
        beds.fit(raw_top, fit_mountain_height);
        let historical_top = fit_mountain_height(raw_top);
        let top = self.mesh.sample(&self.height, dir);
        let lava_m = if self.edifice_lift(dir) > 12.0 {
            (top - fit_mountain_height(self.volcanic_base(dir))).max(0.0)
        } else { 0.0 };
        let sediment_top = top - lava_m;
        let carbonate = self.mesh.sample(&self.carbonate, dir);
        if sediment_top < historical_top {
            beds.trim(historical_top - sediment_top);
        } else {
            beds.deposit(history_duration_myr(cfg), sediment_top - historical_top, self.mesh.sample(&self.rain, dir),
                self.mesh.sample(&self.temperature, dir), carbonate, false, sediment_top < SEA + 2.0);
        }
        let column = drift::history::RockColumn { beds, top, lava_m, carbonate, weathered_myr };
        HISTORY_ROCK.with(|cache| *cache.borrow_mut() = Some((key, column)));
        Some(column)
    }

}

fn same_column(actual: &Option<drift::history::RockColumn>, expected: &Option<drift::history::RockColumn>) {
    match (actual, expected) {
        (Some(a), Some(b)) => {
            a.beds.assert_history_stencil_bits_eq(&b.beds);
            assert_eq!([a.top, a.lava_m, a.carbonate, a.weathered_myr].map(f64::to_bits),
                [b.top, b.lava_m, b.carbonate, b.weathered_myr].map(f64::to_bits));
        }
        (None, None) => {}
        _ => panic!("RockColumn presence differs"),
    }
}

fn clear_memo() { HISTORY_ROCK.with(|memo| *memo.borrow_mut() = None); }

fn check_directions(world: &World, cfg: &WorldSampleConfig, directions: &[DVec3]) -> usize {
    let mut some = 0;
    for &dir in directions {
        clear_memo();
        let expected = world.history_strata_reference(cfg, dir);
        clear_memo();
        let actual = world.history_strata(cfg, dir);
        same_column(&actual, &expected);
        let hit = world.history_strata(cfg, dir);
        same_column(&hit, &expected);
        HISTORY_ROCK.with(|memo| {
            if let Some((key, cached)) = &*memo.borrow() {
                assert_eq!(key.1, dir.to_array().map(f64::to_bits));
                same_column(&Some(*cached), &expected);
            } else { assert!(expected.is_none()); }
        });
        some += usize::from(actual.is_some());
    }
    some
}

fn force_history_branches(world: &mut World, cfg: &WorldSampleConfig, dir: DVec3) {
    let n = world.height.len();
    let history = std::mem::replace(&mut world.history_top, vec![200.0; n]);
    let height = std::mem::replace(&mut world.height, vec![50.0; n]);
    let edifice = std::mem::replace(&mut world.edifice_lift, vec![0.0; n]);
    for (label, top, lift) in [("trim", 50.0, 0.0), ("deposit", 250.0, 0.0),
        ("lava", 600.0, 20.0)] {
        world.height.fill(top);
        world.edifice_lift.fill(lift);
        clear_memo();
        let expected = world.history_strata_reference(cfg, dir).expect("real historical fixture");
        let historical_top = fit_mountain_height(world.mesh.sample(&world.history_top, dir));
        let sediment_top = expected.top - expected.lava_m;
        match label {
            "trim" => { assert!(sediment_top < historical_top); assert_eq!(expected.lava_m, 0.0); }
            "deposit" => { assert!(sediment_top >= historical_top); assert_eq!(expected.lava_m, 0.0); }
            "lava" => { assert!(world.edifice_lift(dir) > 12.0); assert!(expected.lava_m > 0.0); }
            _ => unreachable!(),
        }
        clear_memo();
        let actual = world.history_strata(cfg, dir);
        same_column(&actual, &Some(expected));
        same_column(&world.history_strata(cfg, dir), &Some(expected));
        eprintln!("[history-stencil-branch] case={label} sediment_top={sediment_top} historical_top={historical_top} lava_m={}",
            expected.lava_m);
    }
    world.history_top = history;
    world.height = height;
    world.edifice_lift = edifice;
    clear_memo();
}
#[test]
#[ignore = "actual-world history interpolation; isolated AXIOM_HISTORY_STENCIL_VERSION/SEED"]
fn history_stencil_reuse_matches_frozen_columns_and_memo() {
    if let Some(path) = std::env::var_os("AXIOM_HISTORY_STENCIL_CACHE") {
        assert!(configure_geology_cache(path.into()));
    }
    let version = std::env::var("AXIOM_HISTORY_STENCIL_VERSION")
        .unwrap_or_else(|_| "287".into()).parse().unwrap();
    assert!([278, 279, 280, 283, 286, 287].contains(&version));
    let cfg = WorldSampleConfig { generator_version: version,
        seed: std::env::var("AXIOM_HISTORY_STENCIL_SEED")
            .unwrap_or_else(|_| "74".into()).parse().unwrap(),
        world_type: WorldType::Planet, planet: None,
        voxel_scale: engine_core::VoxelScalePreset::OneMeter };
    let mut world = build(&cfg, &geo::Progress::default(), true);
    // Uniform globe coverage plus exact seams/corners and near-seam interiors.
    let mut directions: Vec<_> = world.mesh.directions.iter()
        .step_by((world.mesh.directions.len() / 96).max(1)).copied().collect();
    for face in Face::ALL {
        for s in [-1.0, -0.999999, 0.0, 0.999999, 1.0] {
            for t in [-1.0, -0.999999, 0.0, 0.999999, 1.0] {
                directions.push(Projection::Tangent.to_sphere(face, s, t));
            }
        }
    }
    // Explicitly include high/low history and volcanic columns when present,
    // rather than hoping the uniform sample visits the conditional branches.
    for field in [&world.history_top, &world.height, &world.edifice_lift] {
        if let Some((i, _)) = field.iter().enumerate().max_by(|a,b| a.1.total_cmp(b.1)) {
            directions.push(world.mesh.directions[i]);
        }
        if let Some((i, _)) = field.iter().enumerate().min_by(|a,b| a.1.total_cmp(b.1)) {
            directions.push(world.mesh.directions[i]);
        }
    }
    let some = check_directions(&world, &cfg, &directions);
    if geological_history(version) { assert_eq!(some, directions.len()); }
    else { assert_eq!(some, 0); }

    // Preserve the optional edifice helper's empty-field zero behavior.
    let edifice = std::mem::take(&mut world.edifice_lift);
    check_directions(&world, &cfg, &directions);
    world.edifice_lift = edifice;

    if geological_history(version) { force_history_branches(&mut world, &cfg, directions[0]); }

    // An empty history field must return None even with a previously warm memo.
    clear_memo();
    let before = world.history_strata(&cfg, directions[0]);
    let history = std::mem::take(&mut world.history_top);
    assert!(world.history_strata_reference(&cfg, directions[0]).is_none());
    assert!(world.history_strata(&cfg, directions[0]).is_none());
    HISTORY_ROCK.with(|memo| {
        let cached = memo.borrow().as_ref().map(|(_, column)| *column);
        same_column(&cached, &before);
    });
    world.history_top = history;
    clear_memo();
    if geological_history(version) {
        assert!(!world.history_top.is_empty());
        world.plates.clear_drift_for_history_stencil_test();
        assert!(world.plates.history_column(directions[0]).is_none());
        assert!(world.history_strata_reference(&cfg, directions[0]).is_none());
        assert!(world.history_strata(&cfg, directions[0]).is_none());
        HISTORY_ROCK.with(|memo| assert!(memo.borrow().is_none()));
    }
    eprintln!("[history-stencil-exact] version={version} seed={} directions={} some={some}",
        cfg.seed, directions.len());
}
