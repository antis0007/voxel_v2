//! Frozen serial references for the bounded parallel history maps.
use super::*;
use crate::{WorldSampleConfig, WorldType};
use engine_core::VoxelScalePreset;

fn mesh() -> &'static Mesh {
    static MESH: OnceLock<Mesh> = OnceLock::new();
    MESH.get_or_init(|| Mesh::with_side(84))
}

fn config(seed: u32, version: u32, custom: bool) -> WorldSampleConfig {
    WorldSampleConfig { seed, generator_version: version, world_type: WorldType::Planet,
        voxel_scale: VoxelScalePreset::OneMeter,
        planet: custom.then(|| {
            let mut settings = engine_voxel::PlanetSettings::default();
            settings.values[4] = 68;
            settings.values[5] = 27;
            settings
        }) }
}

fn fixture(cfg: &WorldSampleConfig) -> Drift {
    let mesh = mesh();
    let crust = mesh.directions.iter().enumerate().map(|(i, dir)| {
        let continental = (0.5 + 0.45 * dir.x).clamp(0.0, 1.0);
        Crust { continental, continental_km: continental * 28.0,
            ocean_age_myr: 90.0 + 70.0 * dir.z, sediment_m: (i % 7) as f64 * 0.15,
            relief_m: dir.y * 80.0 - 20.0, strain: 0.8 + 0.6 * dir.z,
            slip_m: dir.x * 3.0, carbonate: 0.4 + 0.3 * dir.y, lineage: 1 }
    }).collect();
    let mut ice = crate::planetary::landscape_cycles(cfg.generator_version)
        .then(|| crate::planetary::drift::ice::Ice::new(mesh.directions.len()));
    if let Some(ice) = &mut ice {
        for (i, dir) in mesh.directions.iter().enumerate() {
            ice.excess[i] = (dir.y.abs() - 0.4).max(0.0) * 9.0;
            ice.baseline[i] = (i % 5) as f64;
            ice.target[i] = (i % 3) as f64;
            ice.former[i] = dir.y.abs();
        }
    }
    Drift { mesh, crust,
        // These maps read only whether history exists, not any history row.
        history: crate::planetary::geological_history(cfg.generator_version)
            .then(|| history::Field::with_duration(0, crate::planetary::history_duration_myr(cfg))),
        sea_m: SEA - 2.0, ice,
        deep: crate::planetary::deep_time(cfg.generator_version)
            .then(|| crate::planetary::drift::deep_time::Evolution::new(cfg.seed)) }
}

fn assert_bits(actual: &[f64], expected: &[f64], label: &str) {
    assert_eq!(actual.len(), expected.len(), "{label}");
    for (i, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual.to_bits(), expected.to_bits(), "{label}, node={i}");
    }
}

#[test]
fn history_map_fixture_exercises_three_ordered_worker_batches() {
    let count = mesh().directions.len();
    assert_eq!(count, 42_338);
    assert_eq!(4_usize.min(count.div_ceil(16_384)), 3);
    // Force the requested cap rather than silently serializing on small CI hosts.
    let ids = crate::planetary::map_vertices(count, 4, |i| (i, std::thread::current().id()));
    let mut threads = std::collections::HashSet::new();
    for (i, (actual, thread)) in ids.into_iter().enumerate() {
        assert_eq!(i, actual);
        threads.insert(thread);
    }
    assert_eq!(threads.len(), 3);
}

#[test]
fn parallel_resolved_height_matches_original_serial_map() {
    for (seed, version, custom) in [(74, 283, false), (731, 286, true), (74, 287, false)] {
        let cfg = config(seed, version, custom);
        let mut plates = Plates::new(&cfg);
        let field = fixture(&cfg);
        let noise: Vec<_> = field.mesh.directions.iter().map(|&dir| plates.province_noise(dir)).collect();
        let warped: Vec<_> = field.mesh.directions.iter().map(|&dir| plates.warp(dir)).collect();
        for phase in 0..2 {
            if phase == 1 { let _ = plates.advance(0.006); }
            let edges: Vec<_> = warped.iter().map(|&q| plates.warped_edge(q)).collect();
            // Original V5 loop, independent of the new resolved_height helper.
            let expected: Vec<_> = field.mesh.directions.iter().enumerate().map(|(i, &dir)| {
                plates.evolved_at_with_noise(dir, &edges[i], &noise[i], |d| field.at(d)).height
            }).collect();
            for workers in [1, 4] {
                let actual = resolved_height(&field, &plates, &edges, &noise, workers);
                assert_bits(&actual, &expected,
                    &format!("height seed={seed}, version={version}, phase={phase}, workers={workers}"));
            }
        }
    }
}

#[test]
fn parallel_weather_matches_original_serial_map_and_state_writes() {
    let mesh = mesh();
    let samples = WeatherSamples::new(mesh);
    for (seed, version, custom, case, myr) in [(74, 278, false, 0, 5.0),
        (74, 283, false, 1, 119.75), (731, 286, true, 2, 120.0), (74, 287, false, 3, 350.25)] {
        let cfg = config(seed, version, custom);
        let plates = Plates::new(&cfg);
        let climate_basis: Vec<_> = mesh.directions.iter().map(|&dir|
            ClimateBasis::new(cfg.seed, &PlanetAxis::EARTHLIKE, dir)).collect();
        let height: Vec<_> = mesh.directions.iter().enumerate().map(|(i, dir)| match case {
            0 => SEA - 3.0,
            1 => SEA + dir.y * 180.0 + dir.x * 25.0,
            2 => SEA + if i % 3 == 0 { -100.0 } else { 250.0 },
            _ => SEA + (i as f64 * 0.31).sin() * 200.0,
        }).collect();
        let mut reference = fixture(&cfg);
        let (expected, expected_sea) = serial_weather_reference(&mut reference, &plates, &samples, &height, myr);
        for workers in [1, 4] {
            let mut field = fixture(&cfg);
            let (actual, sea) = weather_at(&mut field, &plates, &samples, &climate_basis, &height, myr, workers);
            assert_eq!(sea.to_bits(), expected_sea.to_bits(), "sea version={version}, workers={workers}");
            assert_eq!(actual.len(), expected.len());
            for (i, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
                assert_eq!(actual.map(f64::to_bits), expected.map(f64::to_bits),
                    "weather version={version}, workers={workers}, node={i}");
            }
            match (&field.ice, &reference.ice) {
                (Some(a), Some(b)) => {
                    assert_bits(&a.baseline, &b.baseline, "ice baseline");
                    assert_bits(&a.target, &b.target, "ice target");
                    assert_bits(&a.excess, &b.excess, "ice excess");
                    assert_bits(&a.former, &b.former, "ice former");
                }
                (None, None) => {}
                _ => panic!("ice presence changed"),
            }
            match (field.deep.as_ref().and_then(|d| d.climate.as_ref()),
                reference.deep.as_ref().and_then(|d| d.climate.as_ref())) {
                (Some(a), Some(b)) => {
                    for band in 0..=48 {
                        let latitude = -1.0 + band as f64 / 24.0;
                        assert_eq!(a.anomaly(latitude).to_bits(), b.anomaly(latitude).to_bits());
                    }
                }
                (None, None) => {}
                _ => panic!("deep climate presence changed"),
            }
        }
    }
}

// Frozen V5 implementation. Keep its serial collect and reductions unchanged.
fn serial_weather_reference(field: &mut Drift, plates: &Plates, weather_samples: &WeatherSamples, height: &[f64], myr: f64) -> (Vec<[f64; 4]>, f64) {
    let mesh = field.mesh;
    let cfg = plates.config();
    let settings = cfg.planet_settings();
    let profile = crate::planet_profile::profile(cfg.seed);
    let historical = field.history.is_some();
    let energy = crate::planetary::landscape_cycles(cfg.generator_version)
        .then(|| crate::planetary::drift::climate_cycle::solve(mesh, height, cfg.seed, myr,
            crate::planetary::history_duration_myr(&cfg), field.deep.as_ref().map(|d| d.forcing())));
    let cooling = if energy.is_some() { 0.0 } else if historical {
        let epochs = [8.0, 0.0, 12.0, 2.0, 6.0, 0.0, 0.0];
        let phase = f64::from(myr) / 40.0;
        let age = (phase.floor() as usize).min(5);
        epochs[age] + (epochs[age + 1] - epochs[age]) * smooth(0.0, 1.0, phase - age as f64)
    } else {
        [8.0, 0.0, 12.0, 2.0, 6.0, 0.0][((myr - 1.0).max(0.0) as usize / 40).min(5)]
    };
    // Compact eustatic approximation for the game's vertical scale. Cooling
    // stores water on land; warm epochs return to the present-day datum.
    let sea = if let Some(ice) = &field.ice { ice.sea(mesh, height) }
    else if crate::planetary::changing_seas(cfg.generator_version) {
        SEA - 6.0 * smooth(0.0, 12.0, cooling)
    } else { SEA };
    let weather: Vec<_> = mesh.directions.iter().enumerate().map(|(i, &dir)| {
        let mut climate = crate::planet_climate::at(cfg.seed, &PlanetAxis::EARTHLIKE, dir,
            ((height[i] - SEA) / 200.0).clamp(0.0, 1.0));
        if let Some(s) = settings {
            climate.wet = ((climate.wet - f64::from(profile.rainfall - 0.5) * 0.3)
                * s.unit(5) * 2.0).clamp(0.0, 1.0);
            let warming = (s.unit(4) - 0.5) * 40.0;
            climate.annual_c += warming;
            climate.winter_c += warming;
        }
        let rise = height[i] - weather_samples.sample(height, i, 0);
        let mut wet = (climate.wet + (rise / 320.0).clamp(-0.3, 0.3)).clamp(0.0, 1.0);
        if historical {
            // Continuous climate epochs; drifting shores and ranges determine
            // maritime recharge and rain shadows within each epoch.
            let mut marine = 0.0;
            let mut barrier = height[i];
            for (sample, weight) in [(1, 0.45), (2, 0.35), (3, 0.20)] {
                let upwind = weather_samples.sample(height, i, sample);
                marine += weight * (1.0 - smooth(sea - 4.0, sea + 12.0, upwind));
                barrier = barrier.max(upwind);
            }
            wet = (wet * 0.74 + marine * 0.34 - ((barrier - height[i]) / 550.0).min(0.30)).clamp(0.0, 1.0);
        }
        let anomaly = energy.as_ref().map_or(-cooling, |e| e.anomaly(dir.y));
        let lapse = if energy.is_some() {
            crate::planet_climate::LAPSE_C_PER_M * (height[i] - sea).max(0.0)
        } else { 0.0 };
        let annual = climate.annual_c + anomaly - lapse;
        [wet * (0.25 + 0.75 * smooth(-6.0, 3.0, annual)), wet, annual, climate.winter_c + anomaly - lapse]
    }).collect();
    if let (Some(ice), Some(energy)) = (&mut field.ice, &energy) {
        for (i, &[_rain, wet, annual, _winter]) in weather.iter().enumerate() {
            let reference = annual - energy.anomaly(mesh.directions[i].y);
            let amount = |temperature: f64| 32.0 * wet * smooth(-1.0, 13.0, -temperature);
            ice.baseline[i] = if height[i] > SEA { amount(reference) } else { 0.0 };
            ice.target[i] = if height[i] > SEA { (amount(annual) - ice.baseline[i]).max(0.0) } else { 0.0 };
        }
    }
    if let Some(deep) = &mut field.deep { deep.climate = energy; }
    (weather, sea)
}
