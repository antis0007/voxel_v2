use super::tests::{TICK, compile, home_def, id, rel};
use super::*;
use crate::data::{RotationDef, RotationPeriod, SystemDef};
use crate::fixtures::{binary, two_body};
use crate::validate::SystemError;

#[test]
fn home_solar_day_is_480_seconds() {
    let system = compile(&home_def());
    let home = id(&system, "home");
    let spin = system.spin(home).unwrap().period_ticks as f64;
    let orbit = system.period_ticks[home.0 as usize] as f64;
    let solar_ticks = 1.0 / (1.0 / spin - 1.0 / orbit);
    assert!(
        (solar_ticks * TICK - 480.0).abs() < TICK,
        "{}",
        solar_ticks * TICK
    );
}

#[test]
fn velocity_is_per_system_second_when_time_scale_is_not_one() {
    let mut def = two_body();
    def.time_scale = 4.0;
    let system = compile(&def);
    let p = id(&system, "p");
    let tick_seconds = TICK * def.time_scale;
    let (mut before, mut at, mut after) = (
        Snapshot::default(),
        Snapshot::default(),
        Snapshot::default(),
    );
    for tick in [1, 777, 12_345] {
        system.evaluate(tick - 1, &mut before);
        system.evaluate(tick, &mut at);
        system.evaluate(tick + 1, &mut after);
        let slope = (after.position(p) - before.position(p)) / 2.0;
        let expected = at.velocity(p) * tick_seconds;
        assert!(rel(slope, expected) < 1e-7, "tick {tick}");
    }
}

#[test]
fn periods_under_16_ticks_are_rejected() {
    let too_short = |def: &SystemDef, who: &str| {
        matches!(
            System::compile(def, TICK),
            Err(SystemError::BadOrbit { body, .. }) if body == who
        )
    };
    let mut fast_orbit = two_body();
    fast_orbit.bodies[1].orbit.as_mut().unwrap().period_s = 15.0 * TICK;
    assert!(too_short(&fast_orbit, "p"));
    fast_orbit.bodies[1].orbit.as_mut().unwrap().period_s = 16.0 * TICK;
    System::compile(&fast_orbit, TICK).unwrap();

    let mut fast_spin = two_body();
    fast_spin.bodies[1].rotation = Some(RotationDef {
        period: RotationPeriod::Seconds(15.0 * TICK),
        tilt_deg: 0.0,
        north_deg: 0.0,
        phase_deg: 0.0,
    });
    assert!(too_short(&fast_spin, "p"));

    let mut fast_pair = binary();
    fast_pair.bodies[0].binary.as_mut().unwrap().orbit.period_s = 5.0 * TICK;
    assert!(too_short(&fast_pair, "pair"));
}
