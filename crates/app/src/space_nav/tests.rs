use super::*;
use glam::Vec3;

fn sys() -> &'static System {
    system().unwrap()
}

fn id(name: &str) -> BodyId {
    sys().id(name).unwrap()
}

#[test]
fn contact_directions_match_the_sky_the_renderer_draws() {
    for (eye, tick) in [(DVec3::new(120.0, 80.0, -40.0), 9_000), (DVec3::new(0.0, 60_000.0, 0.0), 123_456)] {
        let fix = Fix::at(sys(), tick, eye).unwrap();
        let sky = crate::planet::camera(eye, true, tick);
        for (slot, name) in [(5, "star"), (6, "moon"), (7, "giant")] {
            let drawn = Vec3::new(sky[slot][0], sky[slot][1], sky[slot][2]);
            let contact = fix.contact(sys(), id(name));
            assert!(contact.dir.angle_between(drawn) < 1e-4, "{name}: {:?} against {drawn:?}", contact.dir);
        }
    }
}

#[test]
fn the_ground_is_the_reference_and_not_a_contact() {
    let fix = Fix::at(sys(), 0, DVec3::new(0.0, 3.0, 0.0)).unwrap();
    assert_eq!(fix.reference, id("home"));
    assert!(fix.altitude_m(sys()) < 10.0);
    let contacts = fix.contacts(sys());
    assert!(contacts.iter().all(|c| c.body != id("home")));
    assert_eq!(contacts.first().map(|c| c.body), Some(id("moon")));
    // Above home's Laplace sphere and inside its Hill sphere the frame is still home's.
    let high = Fix::at(sys(), 0, DVec3::new(0.0, 80_000.0, 0.0)).unwrap();
    assert_eq!(high.reference, id("home"));
    assert!(high.contacts(sys()).iter().any(|c| c.body == id("home")));
}

#[test]
fn pick_prefers_the_body_ahead_then_steps_outward_and_clears() {
    let c = |body: u16, dir: Vec3, range_m: f64| Contact { body: BodyId(body), dir, range_m, angular_radius: 0.01 };
    let contacts = [c(2, Vec3::X, 1.0e4), c(1, Vec3::Y, 1.0e6), c(3, Vec3::Z, 1.0e8)];
    assert_eq!(pick(&contacts, Vec3::Z, None), Some(BodyId(3)));
    // Already targeted and still ahead: step outward instead.
    assert_eq!(pick(&contacts, Vec3::X, Some(BodyId(2))), Some(BodyId(1)));
    let away = Vec3::new(-1.0, -1.0, -1.0).normalize();
    assert_eq!(pick(&contacts, away, None), Some(BodyId(2)));
    assert_eq!(pick(&contacts, away, Some(BodyId(1))), Some(BodyId(3)));
    assert_eq!(pick(&contacts, away, Some(BodyId(3))), None);
}

#[test]
fn closing_speed_follows_the_range_rate() {
    let body = BodyId(9);
    assert_eq!(closing_mps(body, 100, 5_000.0), 0.0);
    let mut last = 0.0;
    for step in 1..=40_u64 {
        // 10 m nearer every 12 ticks is 100 m/s.
        last = closing_mps(body, 100 + step * 12, 5_000.0 - step as f64 * 10.0);
    }
    assert!((last - 100.0).abs() < 1.0, "closing {last}");
    assert_eq!(closing_mps(body, 100 + 40 * 12, 0.0), last);
    assert_eq!(closing_mps(body, 100_000, 4_000.0), 0.0);
}

#[test]
fn transfers_resolve_from_the_ground_and_from_the_moon() {
    let (home, moon, giant, star) = (id("home"), id("moon"), id("giant"), id("star"));
    let to_moon = transfer(sys(), home, moon, 0).unwrap();
    assert!(to_moon.child && to_moon.window_s.is_none());
    assert!(to_moon.depart_mps > 0.0 && to_moon.capture_mps > 0.0 && to_moon.coast_s > 0.0);
    let to_giant = transfer(sys(), moon, giant, 0).unwrap();
    assert_eq!(to_giant.from, home);
    let wait = to_giant.window_s.unwrap();
    assert!(wait >= 0.0 && wait.is_finite());
    // The window found is a window: one tick before it the wait is one tick.
    let at = ((wait * TICKS_PER_SECOND).round() as u64).saturating_sub(1);
    assert!(transfer(sys(), home, giant, at).unwrap().window_s.unwrap() < 1.0);
    assert!(transfer(sys(), moon, home, 0).is_none());
    assert!(transfer(sys(), star, home, 0).is_none());
    assert!(transfer(sys(), home, home, 0).is_none());
}
