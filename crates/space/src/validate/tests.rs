use super::*;
use crate::data::{BodyKind, RotationDef, RotationPeriod, parse};
use crate::fixtures::{binary, binary_around_star, body, orbit, system, two_body};

fn is_bad_orbit(r: Result<(), SystemError>, who: &str) -> bool {
    matches!(r, Err(SystemError::BadOrbit { body, .. }) if body == who)
}

#[test]
fn fixtures_are_valid() {
    validate(&two_body()).unwrap();
    validate(&binary()).unwrap();
    validate(&binary_around_star()).unwrap();
}

#[test]
fn invalid_systems_are_typed_errors() {
    let mut dup = two_body();
    dup.bodies[1].id = "s".into();
    assert!(matches!(validate(&dup), Err(SystemError::DuplicateId(id)) if id == "s"));

    let mut orphan = two_body();
    orphan.bodies[1].parent = Some("nowhere".into());
    assert!(matches!(
        validate(&orphan),
        Err(SystemError::UnknownParent { body, parent }) if body == "p" && parent == "nowhere"
    ));

    let mut cycle = two_body();
    cycle
        .bodies
        .push(body("a", BodyKind::Planet, Some("b"), Some(orbit(10.0))));
    cycle
        .bodies
        .push(body("b", BodyKind::Planet, Some("a"), Some(orbit(10.0))));
    assert!(matches!(validate(&cycle), Err(SystemError::Cycle(id)) if id == "a" || id == "b"));

    let mut missing = binary();
    missing.bodies[0].binary.as_mut().unwrap().b = "missing".into();
    assert!(matches!(validate(&missing), Err(SystemError::BadBinary(id)) if id == "pair"));

    let mut parabolic = two_body();
    parabolic.bodies[1].orbit.as_mut().unwrap().e = 1.0;
    assert!(is_bad_orbit(validate(&parabolic), "p"));

    let mut frozen = two_body();
    frozen.bodies[1].orbit.as_mut().unwrap().period_s = 0.0;
    assert!(is_bad_orbit(validate(&frozen), "p"));

    assert!(matches!(parse("not ron ("), Err(SystemError::Parse(_))));
}

#[test]
fn a_self_parent_is_a_cycle() {
    let mut s = two_body();
    s.bodies[1].parent = Some("p".into());
    assert!(matches!(validate(&s), Err(SystemError::Cycle(id)) if id == "p"));
}

#[test]
fn a_system_has_exactly_one_root() {
    assert!(matches!(
        validate(&system(vec![])),
        Err(SystemError::Roots(0))
    ));
    let mut two_roots = two_body();
    two_roots.bodies[1].parent = None;
    two_roots.bodies[1].orbit = None;
    assert!(matches!(validate(&two_roots), Err(SystemError::Roots(2))));
}

#[test]
fn eccentricity_and_period_have_bounds() {
    for e in [0.0, 0.999] {
        let mut s = two_body();
        s.bodies[1].orbit.as_mut().unwrap().e = e;
        validate(&s).unwrap();
    }
    for e in [-0.01, 0.9991, f64::NAN, f64::INFINITY] {
        let mut s = two_body();
        s.bodies[1].orbit.as_mut().unwrap().e = e;
        assert!(is_bad_orbit(validate(&s), "p"), "e = {e}");
    }
    for period in [-1.0, f64::NAN, f64::INFINITY] {
        let mut s = two_body();
        s.bodies[1].orbit.as_mut().unwrap().period_s = period;
        assert!(is_bad_orbit(validate(&s), "p"), "period = {period}");
    }
    let mut s = two_body();
    s.bodies[1].orbit.as_mut().unwrap().node_deg = f64::NAN;
    assert!(is_bad_orbit(validate(&s), "p"));
}

#[test]
fn mass_radius_temperature_and_time_scale_are_checked() {
    let mut s = two_body();
    s.bodies[1].mass_kg = 0.0;
    validate(&s).unwrap();
    s.bodies[1].mass_kg = -1.0;
    assert!(is_bad_orbit(validate(&s), "p"));

    let mut s = two_body();
    s.bodies[1].radius_m = f64::NAN;
    assert!(is_bad_orbit(validate(&s), "p"));

    let mut s = two_body();
    s.bodies[0].kind = BodyKind::Star { temperature_k: 0.0 };
    assert!(is_bad_orbit(validate(&s), "s"));

    for scale in [0.0, -2.0, f64::NAN] {
        let mut s = two_body();
        s.time_scale = scale;
        assert!(is_bad_orbit(validate(&s), "test"), "time_scale = {scale}");
    }
}

#[test]
fn orbit_presence_follows_the_hierarchy() {
    let mut rootless_orbit = two_body();
    rootless_orbit.bodies[0].orbit = Some(orbit(10.0));
    assert!(is_bad_orbit(validate(&rootless_orbit), "s"));

    let mut no_orbit = two_body();
    no_orbit.bodies[1].orbit = None;
    assert!(is_bad_orbit(validate(&no_orbit), "p"));

    let mut stray = binary();
    stray.bodies[3].orbit = None;
    assert!(is_bad_orbit(validate(&stray), "p"));
}

#[test]
fn binary_definitions_are_checked() {
    let mut no_binary = binary();
    no_binary.bodies[0].binary = None;
    assert!(matches!(validate(&no_binary), Err(SystemError::BadBinary(id)) if id == "pair"));

    let mut stray_binary = binary();
    stray_binary.bodies[3].binary = stray_binary.bodies[0].binary.clone();
    assert!(matches!(validate(&stray_binary), Err(SystemError::BadBinary(id)) if id == "p"));

    let mut same = binary();
    same.bodies[0].binary.as_mut().unwrap().b = "a".into();
    assert!(matches!(validate(&same), Err(SystemError::BadBinary(id)) if id == "pair"));

    let mut elsewhere = binary();
    elsewhere.bodies[2].parent = Some("a".into());
    assert!(matches!(validate(&elsewhere), Err(SystemError::BadBinary(id)) if id == "pair"));

    let mut own_orbit = binary();
    own_orbit.bodies[1].orbit = Some(orbit(10.0));
    assert!(matches!(validate(&own_orbit), Err(SystemError::BadBinary(id)) if id == "pair"));

    let mut bad_shared = binary();
    bad_shared.bodies[0].binary.as_mut().unwrap().orbit.e = 1.5;
    assert!(is_bad_orbit(validate(&bad_shared), "pair"));
}

#[test]
fn locked_spin_needs_an_orbit_or_a_binary_partner() {
    let locked = RotationDef {
        period: RotationPeriod::Locked,
        tilt_deg: 0.0,
        north_deg: 0.0,
        phase_deg: 0.0,
    };
    let mut s = two_body();
    s.bodies[1].rotation = Some(locked.clone());
    validate(&s).unwrap();
    s.bodies[0].rotation = Some(locked.clone());
    assert!(is_bad_orbit(validate(&s), "s"));

    let mut b = binary();
    b.bodies[0].rotation = Some(locked);
    assert!(is_bad_orbit(validate(&b), "pair"));
}

#[test]
fn binary_components_may_lock_to_their_partner() {
    let locked = RotationDef {
        period: RotationPeriod::Locked,
        tilt_deg: 0.0,
        north_deg: 0.0,
        phase_deg: 0.0,
    };
    let mut b = binary();
    b.bodies[1].rotation = Some(locked.clone());
    b.bodies[2].rotation = Some(locked.clone());
    validate(&b).unwrap();
    let mut nested = binary_around_star();
    nested.bodies[0].rotation = Some(locked);
    validate(&nested).unwrap();
}

#[test]
fn locked_spin_rejects_a_tilt_but_ignores_north() {
    let mut s = two_body();
    s.bodies[1].rotation = Some(RotationDef {
        period: RotationPeriod::Locked,
        tilt_deg: 0.0,
        north_deg: 75.0,
        phase_deg: 0.0,
    });
    validate(&s).unwrap();
    s.bodies[1].rotation.as_mut().unwrap().tilt_deg = 5.0;
    assert!(is_bad_orbit(validate(&s), "p"));
    let mut b = binary();
    b.bodies[1].rotation = s.bodies[1].rotation.clone();
    assert!(is_bad_orbit(validate(&b), "a"));
}

#[test]
fn spin_values_are_checked() {
    for period in [0.0, -480.0, f64::NAN] {
        let mut s = two_body();
        s.bodies[1].rotation = Some(RotationDef {
            period: RotationPeriod::Seconds(period),
            tilt_deg: 0.0,
            north_deg: 0.0,
            phase_deg: 0.0,
        });
        assert!(is_bad_orbit(validate(&s), "p"), "spin = {period}");
    }
    let mut s = two_body();
    s.bodies[1].rotation = Some(RotationDef {
        period: RotationPeriod::Seconds(480.0),
        tilt_deg: f64::INFINITY,
        north_deg: 0.0,
        phase_deg: 0.0,
    });
    assert!(is_bad_orbit(validate(&s), "p"));
}

#[test]
fn errors_display_the_offending_id() {
    let e = SystemError::UnknownParent {
        body: "p".into(),
        parent: "nowhere".into(),
    };
    assert_eq!(e.to_string(), "body `p` names unknown parent `nowhere`");
    let e = SystemError::BadOrbit {
        body: "p".into(),
        reason: "period_s must be finite and positive",
    };
    assert_eq!(e.to_string(), "`p`: period_s must be finite and positive");
}

#[test]
fn locked_spin_rejects_a_phase() {
    let mut s = two_body();
    s.bodies[1].rotation = Some(RotationDef {
        period: RotationPeriod::Locked,
        tilt_deg: 0.0,
        north_deg: 0.0,
        phase_deg: 10.0,
    });
    assert!(is_bad_orbit(validate(&s), "p"));
    s.bodies[1].rotation = Some(RotationDef {
        period: RotationPeriod::Seconds(480.0),
        tilt_deg: 0.0,
        north_deg: 0.0,
        phase_deg: 10.0,
    });
    validate(&s).unwrap();
    s.bodies[1].rotation.as_mut().unwrap().phase_deg = f64::NAN;
    assert!(is_bad_orbit(validate(&s), "p"));
}
