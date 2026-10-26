use crate::data::{BodyKind, RotationDef, RotationPeriod, SystemDef};
use crate::fixtures::{binary_around_star, body, orbit, star, system, two_body};
use crate::system::{Snapshot, System};
use glam::DVec3;
use std::f64::consts::FRAC_PI_2;

const TICK: f64 = 1.0 / 120.0;

fn rotation(period: RotationPeriod, tilt_deg: f64, north_deg: f64) -> Option<RotationDef> {
    Some(RotationDef {
        period,
        tilt_deg,
        north_deg,
        phase_deg: 0.0,
    })
}

fn compile(def: &SystemDef) -> System {
    System::compile(def, TICK).unwrap()
}

/// Checks that body `b` turns +X toward `target` at 64 ticks over one period.
fn assert_faces(system: &System, b: &str, target: &str) {
    let (b, target) = (system.id(b).unwrap(), system.id(target).unwrap());
    let spin = system.spin(b).unwrap();
    assert!(spin.locked);
    let mut snap = Snapshot::default();
    for k in 0..64u64 {
        let tick = 3 + k * spin.period_ticks / 64;
        system.evaluate(tick, &mut snap);
        let toward = (snap.position[target.0 as usize] - snap.position[b.0 as usize]).normalize();
        let local = system.orientation(b, tick).inverse() * toward;
        assert!(
            (local - DVec3::X).length() < 1e-9,
            "tick {tick}: +X misses the target by {local:?}"
        );
    }
}

#[test]
fn spin_period_returns_to_identity_rotation() {
    let mut def = two_body();
    def.bodies[1].rotation = rotation(RotationPeriod::Seconds(480.0), 23.4, 30.0);
    let system = compile(&def);
    let p = system.id("p").unwrap();
    let spin_ticks = system.spin(p).unwrap().period_ticks;
    assert_eq!(spin_ticks, 57_600);
    for tick in [0, 1, 777, 57_599, 1_000_000_000_007] {
        let now = system.orientation(p, tick);
        let later = system.orientation(p, tick + spin_ticks);
        assert!(now.abs_diff_eq(later, 1e-12), "tick {tick}");
    }
    // A quarter turn later the body frame has moved.
    let quarter = system.orientation(p, spin_ticks / 4);
    assert!(!system.orientation(p, 0).abs_diff_eq(quarter, 1e-3));
}

#[test]
fn locked_moon_faces_its_parent_all_orbit() {
    let mut moon_orbit = orbit(1_440.0);
    moon_orbit.e = 0.0;
    moon_orbit.i_deg = 5.1;
    moon_orbit.node_deg = 40.0;
    moon_orbit.peri_deg = 40.0;
    moon_orbit.mean_anomaly_deg = 75.0;
    let mut moon = body("m", BodyKind::Moon, Some("p"), Some(moon_orbit));
    moon.rotation = rotation(RotationPeriod::Locked, 0.0, 0.0);
    let mut def = two_body();
    def.bodies[0].mass_kg = 1.0e30;
    def.bodies[1].mass_kg = 6.0e24;
    def.bodies.push(moon);
    let system = compile(&def);
    let m = system.id("m").unwrap();
    assert_eq!(system.spin(m).unwrap().period_ticks, 172_800);
    assert_faces(&system, "m", "p");
    assert_axis_is_orbit_normal(&system, "m", 5.1, 40.0);
}

#[test]
fn locked_moon_on_a_retrograde_orbit_ignores_north_and_faces_its_parent() {
    let mut moon_orbit = orbit(900.0);
    moon_orbit.e = 0.0;
    moon_orbit.i_deg = 160.0;
    moon_orbit.node_deg = 70.0;
    moon_orbit.mean_anomaly_deg = 200.0;
    let mut moon = body("m", BodyKind::Moon, Some("s"), Some(moon_orbit));
    moon.rotation = rotation(RotationPeriod::Locked, 0.0, -125.0);
    let def = system(vec![body("s", star(), None, None), moon]);
    let system = compile(&def);
    assert_faces(&system, "m", "s");
    assert_axis_is_orbit_normal(&system, "m", 160.0, 70.0);
}

/// Checks that body `b`'s spin axis is the normal of an orbit with `i` and `node`.
fn assert_axis_is_orbit_normal(system: &System, b: &str, i_deg: f64, node_deg: f64) {
    let (i, node) = (i_deg.to_radians(), node_deg.to_radians());
    let normal = DVec3::new(i.sin() * node.sin(), i.cos(), i.sin() * node.cos());
    let b = system.id(b).unwrap();
    for tick in [0, 999, 123_456] {
        let axis = system.orientation(b, tick) * DVec3::Y;
        assert!((axis - normal).length() < 1e-12, "tick {tick}: {axis:?}");
    }
}

#[test]
fn locked_binary_components_face_each_other() {
    let mut def = binary_around_star();
    let shared = &mut def.bodies[2].binary.as_mut().unwrap().orbit;
    shared.e = 0.0;
    let (i, node) = (shared.i_deg, shared.node_deg);
    for c in &mut def.bodies[..2] {
        c.rotation = rotation(RotationPeriod::Locked, 0.0, 0.0);
    }
    let system = compile(&def);
    let pair = system.id("pair").unwrap();
    let a = system.id("a").unwrap();
    let b = system.id("b").unwrap();
    // Both lock to the pair's shared period of 1e6 s.
    assert_eq!(system.spin(a).unwrap().period_ticks, 120_000_000);
    assert_eq!(system.spin(b).unwrap().period_ticks, 120_000_000);
    assert_faces(&system, "a", "b");
    assert_faces(&system, "b", "a");
    assert_faces(&system, "a", system.name(pair));
    assert_axis_is_orbit_normal(&system, "a", i, node);
    assert_axis_is_orbit_normal(&system, "b", i, node);
}

#[test]
fn tilt_and_north_set_the_spin_axis() {
    let mut def = two_body();
    def.bodies[1].rotation = rotation(RotationPeriod::Seconds(480.0), 23.4, 30.0);
    let system = compile(&def);
    let p = system.id("p").unwrap();
    let (tilt, north) = (23.4f64.to_radians(), 30.0f64.to_radians());
    let expected = DVec3::new(
        tilt.sin() * north.cos(),
        tilt.cos(),
        -tilt.sin() * north.sin(),
    );
    for tick in [0, 5, 14_400, 40_001] {
        let axis = system.orientation(p, tick) * DVec3::Y;
        assert!((axis - expected).length() < 1e-12, "tick {tick}: {axis:?}");
    }
}

#[test]
fn a_body_without_rotation_keeps_the_system_frame() {
    let system = compile(&two_body());
    let p = system.id("p").unwrap();
    assert_eq!(system.orientation(p, 12_345), glam::DQuat::IDENTITY);
}

#[test]
fn authored_phase_sets_the_spin_angle_at_tick_zero() {
    let mut turned = rotation(RotationPeriod::Seconds(480.0), 23.4, 30.0).unwrap();
    turned.phase_deg = 90.0;
    let mut def = two_body();
    def.bodies[1].rotation = Some(turned);
    let system = compile(&def);
    let p = system.id("p").unwrap();
    let axis_frame = system.orientation(p, 0) * glam::DQuat::from_rotation_y(-FRAC_PI_2);
    let mut plain = two_body();
    plain.bodies[1].rotation = rotation(RotationPeriod::Seconds(480.0), 23.4, 30.0);
    let reference = compile(&plain).orientation(p, 0);
    assert!(axis_frame.angle_between(reference) < 1e-12);
    let quarter = system.spin(p).unwrap().period_ticks / 4;
    let advanced = compile(&plain).orientation(p, quarter);
    assert!(system.orientation(p, 0).angle_between(advanced) < 1e-9);
}
