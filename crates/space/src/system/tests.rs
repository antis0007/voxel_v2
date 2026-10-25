use super::*;
use crate::data::{BodyKind, SystemDef, parse};
use crate::fixtures::{binary, binary_around_star, body, orbit, star, system, two_body};
use crate::kepler::{Elements, G};
use crate::validate::SystemError;
use std::f64::consts::TAU;

const TICK: f64 = 1.0 / 120.0;

fn home_def() -> SystemDef {
    let text = include_str!("../../../../assets/star_systems.ron");
    parse(text).unwrap().systems.remove(0)
}

fn compile(def: &SystemDef) -> System {
    System::compile(def, TICK).unwrap()
}

fn id(system: &System, name: &str) -> BodyId {
    system.id(name).unwrap()
}

fn assert_parent_first(system: &System) {
    for i in 0..system.len() {
        let b = BodyId(i as u16);
        if let Some(p) = system.parent(b) {
            assert!(p.0 < b.0, "{} precedes its parent", system.name(b));
        }
    }
}

fn rel(value: DVec3, reference: DVec3) -> f64 {
    (value - reference).length() / reference.length()
}

fn phase(tick: u64, period_ticks: u64) -> f64 {
    TAU * ((tick % period_ticks) as f64 / period_ticks as f64)
}

#[test]
fn home_system_compiles_parent_first() {
    let system = compile(&home_def());
    assert_eq!(system.len(), 4);
    assert_parent_first(&system);
    assert_eq!(system.parent(id(&system, "star")), None);
    assert_eq!(
        system.parent(id(&system, "moon")),
        Some(id(&system, "home"))
    );
    assert_eq!(system.id("nobody"), None);
}

#[test]
fn child_listed_before_parent_still_compiles_parent_first() {
    let def = system(vec![
        body("m", BodyKind::Moon, Some("p"), Some(orbit(77.0))),
        body("p", BodyKind::Planet, Some("s"), Some(orbit(1000.0))),
        body("s", star(), None, None),
    ]);
    let system = compile(&def);
    assert_parent_first(&system);
    assert_eq!(system.name(BodyId(0)), "s");
    assert_eq!(system.name(id(&system, "m")), "m");
    assert_eq!(system.parent(id(&system, "m")), Some(id(&system, "p")));
}

#[test]
fn moon_relative_position_matches_its_elements_same_tick() {
    let def = home_def();
    let system = compile(&def);
    let (home, moon) = (id(&system, "home"), id(&system, "moon"));
    let data = def.bodies.iter().find(|b| b.id == "moon").unwrap();
    let o = data.orbit.as_ref().unwrap();
    let mu = G * (system.mass(home) + system.mass(moon));
    let el = Elements::from_period(
        o.period_s,
        mu,
        o.e,
        o.i_deg.to_radians(),
        o.node_deg.to_radians(),
        o.peri_deg.to_radians(),
    );
    let mut snap = Snapshot::default();
    for tick in [0, 1, 123_457, 172_799, 9_000_000_001] {
        system.evaluate(tick, &mut snap);
        let m = o.mean_anomaly_deg.to_radians() + phase(tick, 172_800);
        let (pos, vel) = el.state_at(m);
        let (h, b) = (home.0 as usize, moon.0 as usize);
        assert!((snap.position[b] - snap.position[h] - pos).length() < 1e-6);
        assert!((snap.velocity[b] - snap.velocity[h] - vel).length() < 1e-6);
    }
}

#[test]
fn long_saves_do_not_lose_phase() {
    let mut def = system(vec![
        body("s", star(), None, None),
        body("p", BodyKind::Planet, Some("s"), Some(orbit(1000.3))),
        body("m", BodyKind::Moon, Some("p"), Some(orbit(77.77))),
    ]);
    def.time_scale = 1.7;
    let system = compile(&def);
    const LONG: u64 = 1_000_000_000_000;
    for name in ["p", "m"] {
        let b = id(&system, name);
        let period = system.period_ticks[b.0 as usize];
        assert_ne!(LONG % period, 0, "{name} period divides 10^12");
        let reduced = LONG % period;
        assert_eq!(
            system.relative_state(b, LONG),
            system.relative_state(b, reduced)
        );
        assert_ne!(
            system.relative_state(b, reduced),
            system.relative_state(b, reduced + 1)
        );
    }
}

#[test]
fn velocity_matches_position_change_per_tick() {
    let system = compile(&home_def());
    for name in ["home", "moon", "giant"] {
        let b = id(&system, name);
        for tick in [1, 50_000, 1_000_003] {
            let (before, _) = system.relative_state(b, tick - 1);
            let (after, _) = system.relative_state(b, tick + 1);
            let (_, vel) = system.relative_state(b, tick);
            let slope = (after - before) / (2.0 * TICK);
            assert!(rel(slope, vel) < 1e-7, "{name} at {tick}");
        }
    }
}

#[test]
fn time_scale_divides_period_ticks() {
    let slow = compile(&two_body());
    let mut fast_def = two_body();
    fast_def.time_scale = 4.0;
    let fast = compile(&fast_def);
    let p = id(&slow, "p");
    assert_eq!(slow.period_ticks[p.0 as usize], 120_000);
    assert_eq!(fast.period_ticks[p.0 as usize], 30_000);
    for tick in [0, 7, 29_999, 1_234_567] {
        let (a, _) = fast.relative_state(p, tick);
        let (b, _) = slow.relative_state(p, 4 * tick);
        assert!((a - b).length() <= 1e-9 * b.length());
    }
}

fn assert_zero_momentum(system: &System, snap: &Snapshot, center: DVec3) {
    let (a, b) = (id(system, "a").0 as usize, id(system, "b").0 as usize);
    let (ma, mb) = (system.mass(BodyId(a as u16)), system.mass(BodyId(b as u16)));
    let (ra, rb) = (snap.position[a] - center, snap.position[b] - center);
    let center_vel = snap.velocity[system.parent(BodyId(a as u16)).unwrap().0 as usize];
    let (va, vb) = (snap.velocity[a] - center_vel, snap.velocity[b] - center_vel);
    assert!((ma * ra + mb * rb).length() <= 1e-9 * ma * ra.length());
    assert!((ma * va + mb * vb).length() <= 1e-9 * ma * va.length());
}

#[test]
fn binary_keeps_its_barycenter_fixed_and_momentum_zero() {
    let mut def = binary();
    def.bodies[1].mass_kg = 2.0e30;
    def.bodies[2].mass_kg = 1.0e30;
    let shared = &mut def.bodies[0].binary.as_mut().unwrap().orbit;
    shared.period_s = 1.0e7;
    shared.e = 0.3;
    let system = compile(&def);
    assert_parent_first(&system);
    let pair = id(&system, "pair");
    assert_eq!(system.mass(pair), 2.0e30 + 1.0e30);
    let relative = Elements::from_period(1.0e7, G * (2.0e30 + 1.0e30), 0.3, 0.0, 0.0, 0.0);
    let period = 1_200_000_000;
    let (a, b) = (id(&system, "a").0 as usize, id(&system, "b").0 as usize);
    let mut snap = Snapshot::default();
    for k in 0..50 {
        let tick = k * period / 50 + 7;
        system.evaluate(tick, &mut snap);
        assert_eq!(snap.position[pair.0 as usize], DVec3::ZERO);
        assert_eq!(snap.velocity[pair.0 as usize], DVec3::ZERO);
        assert_zero_momentum(&system, &snap, DVec3::ZERO);
        let (separation, _) = relative.state_at(phase(tick, period));
        assert!(rel(snap.position[a] - snap.position[b], separation) < 1e-9);
    }
}

#[test]
fn barycenter_orbiting_a_star_carries_its_pair() {
    let def = binary_around_star();
    let system = compile(&def);
    assert_parent_first(&system);
    let (s, pair) = (id(&system, "s"), id(&system, "pair"));
    assert_eq!(system.parent(pair), Some(s));
    assert_eq!(system.parent(id(&system, "a")), Some(pair));
    assert_eq!(system.mass(pair), 1.0e27 + 5.0e26);
    let own = Elements::from_period(3.0e7, G * (2.0e30 + (1.0e27 + 5.0e26)), 0.1, 0.0, 0.0, 0.0);
    let period = 3_600_000_000;
    let mut snap = Snapshot::default();
    let mut previous = None;
    for k in 0..12 {
        let tick = k * period / 12 + 11;
        system.evaluate(tick, &mut snap);
        let center = snap.position[pair.0 as usize];
        assert!(rel(center, own.state_at(phase(tick, period)).0) < 1e-9);
        assert_ne!(previous, Some(center));
        previous = Some(center);
        assert_zero_momentum(&system, &snap, center);
    }
}

#[test]
fn binary_components_need_positive_mass() {
    let mut def = binary_around_star();
    def.bodies[1].mass_kg = 0.0;
    assert!(matches!(
        System::compile(&def, TICK),
        Err(SystemError::BadBinary(id)) if id == "pair"
    ));
}

#[test]
fn tick_length_must_be_positive() {
    for tick in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            System::compile(&two_body(), tick),
            Err(SystemError::BadOrbit { body, .. }) if body == "test"
        ));
    }
}

#[test]
fn home_planet_surface_gravity_is_provisional_target() {
    let system = compile(&home_def());
    let g = system.surface_gravity(id(&system, "home"));
    assert!((g - 9.81).abs() < 0.01 * 9.81, "{g}");
    let pair = compile(&binary());
    assert_eq!(pair.surface_gravity(id(&pair, "pair")), 0.0);
}

#[test]
fn evaluate_reuses_snapshot_storage() {
    let system = compile(&home_def());
    let mut snap = Snapshot::default();
    system.evaluate(0, &mut snap);
    let pointers = (snap.position.as_ptr(), snap.velocity.as_ptr());
    let capacities = (snap.position.capacity(), snap.velocity.capacity());
    system.evaluate(987_654_321, &mut snap);
    assert_eq!((snap.position.as_ptr(), snap.velocity.as_ptr()), pointers);
    assert_eq!(
        (snap.position.capacity(), snap.velocity.capacity()),
        capacities
    );
    assert_eq!(snap.position.len(), system.len());
    assert_eq!(snap.position[0], DVec3::ZERO);
}
