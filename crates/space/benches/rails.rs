//! Benchmarks one 64-body tick evaluation and a batch of Kepler solves.
//! `just bench-check` compares both with `baselines.json`.
use criterion::{Criterion, criterion_group, criterion_main};
use engine_space::{BodyDef, BodyKind, OrbitDef, Snapshot, System, SystemDef, solve_kepler};
use std::hint::black_box;
use std::time::Duration;

const TICK_SECONDS: f64 = 1.0 / 120.0;
/// Tick used for evaluation; about 8.3e6 s of system time at 120 ticks per second.
const LARGE_TICK: u64 = 1_000_000_000;
const PLANETS: usize = 9;
const MOONS_PER_PLANET: usize = 2;
const SUB_MOONS_PER_MOON: usize = 2;
const SOLVES: usize = 100_000;

/// Maps a key to `[0, 1)` with a splitmix64 finalizer, so every run matches.
fn unit(key: u64) -> f64 {
    let mut z = key.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// Orbit with `e` in 0 to 0.3 and varied angles, from a per-body key.
fn orbit(key: u64, period_s: f64) -> OrbitDef {
    OrbitDef {
        period_s,
        e: 0.3 * unit(key),
        i_deg: 12.0 * unit(key + 1),
        node_deg: 360.0 * unit(key + 2),
        peri_deg: 360.0 * unit(key + 3),
        mean_anomaly_deg: 360.0 * unit(key + 4),
    }
}

fn body(id: String, kind: BodyKind, parent: &str, mass_kg: f64, orbit: OrbitDef) -> BodyDef {
    BodyDef {
        id,
        kind,
        mass_kg,
        radius_m: 1.0e6,
        parent: Some(parent.to_string()),
        orbit: Some(orbit),
        binary: None,
        rotation: None,
    }
}

/// One star, 9 planets, 18 moons and 36 sub-moons: 64 bodies, chains of depth 3.
fn fixture() -> SystemDef {
    let mut bodies = vec![BodyDef {
        id: "star".into(),
        kind: BodyKind::Star {
            temperature_k: 5800.0,
        },
        mass_kg: 2.0e30,
        radius_m: 7.0e8,
        parent: None,
        orbit: None,
        binary: None,
        rotation: None,
    }];
    let mut key = 0u64;
    let mut next = || {
        key += 16;
        key
    };
    for p in 0..PLANETS {
        let planet = format!("p{p}");
        let kind = if p >= 5 {
            BodyKind::GasGiant
        } else {
            BodyKind::Planet
        };
        let mass = 3.0e24 * 3.0f64.powi(p as i32 / 2 + 1);
        let period = 1.0e7 * 1.6f64.powi(p as i32);
        bodies.push(body(
            planet.clone(),
            kind,
            "star",
            mass,
            orbit(next(), period),
        ));
        for m in 0..MOONS_PER_PLANET {
            let moon = format!("{planet}m{m}");
            let period = 2.0e5 * (1.0 + m as f64 + unit(next()));
            let mass = 1.0e21 * (1.0 + 4.0 * unit(next()));
            bodies.push(body(
                moon.clone(),
                BodyKind::Moon,
                &planet,
                mass,
                orbit(next(), period),
            ));
            for s in 0..SUB_MOONS_PER_MOON {
                let sub = format!("{moon}s{s}");
                let period = 1.0e4 * (1.0 + s as f64 + unit(next()));
                let mass = 1.0e17 * (1.0 + 9.0 * unit(next()));
                bodies.push(body(
                    sub,
                    BodyKind::Moon,
                    &moon,
                    mass,
                    orbit(next(), period),
                ));
            }
        }
    }
    SystemDef {
        id: "bench".into(),
        time_scale: 1.0,
        bodies,
    }
}

fn evaluate_64_bodies(c: &mut Criterion) {
    let system = System::compile(&fixture(), TICK_SECONDS).expect("fixture is valid");
    assert_eq!(system.len(), 64);
    let mut snapshot = Snapshot::default();
    c.bench_function("rails/evaluate_64_bodies", |b| {
        b.iter(|| {
            system.evaluate(black_box(LARGE_TICK), &mut snapshot);
            black_box(snapshot.position[63])
        })
    });
}

/// `(M, e)` with M over several turns and e in 0 to 0.97, a stress range.
///
/// Validation allows e up to 0.999; the home data stays far below 0.97.
fn solve_pairs() -> Vec<(f64, f64)> {
    (0..SOLVES as u64)
        .map(|k| (60.0 * unit(2 * k) - 20.0, 0.97 * unit(2 * k + 1)))
        .collect()
}

fn solve_kepler_100k(c: &mut Criterion) {
    let pairs = solve_pairs();
    c.bench_function("rails/solve_kepler_100k", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for &(m, e) in &pairs {
                sum += solve_kepler(black_box(m), black_box(e));
            }
            black_box(sum)
        })
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(30).warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2));
    targets = evaluate_64_bodies, solve_kepler_100k
}
criterion_main!(benches);
