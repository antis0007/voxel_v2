use super::*;
use crate::data::parse;

const TICK: f64 = 1.0 / 120.0;
const MU_EARTH: f64 = 3.986_004_418e14;
const DAY: f64 = 86_400.0;

fn home() -> System {
    let text = include_str!("../../../../assets/star_systems.ron");
    let def = parse(text).unwrap().systems.remove(0);
    System::compile(&def, TICK).unwrap()
}

fn close(value: f64, expected: f64, tolerance: f64) {
    assert!(
        (value - expected).abs() <= tolerance,
        "{value} differs from {expected} by more than {tolerance}"
    );
}

// Curtis, Orbital Mechanics for Engineering Students, example 6.1 class:
// 300 km LEO to geostationary radius.
#[test]
fn leo_to_geo_matches_textbook() {
    let h = Hohmann::new(MU_EARTH, 6_678e3, 42_164e3);
    close(h.depart, 2_425.0, 5.0);
    close(h.arrive, 1_466.0, 5.0);
    close(h.time_s / 3_600.0, 5.29, 0.02);
}

#[test]
fn earth_sphere_of_influence_is_925_000_km() {
    close(soi_radius(1.496e11, 5.972e24, 1.989e30), 9.25e8, 0.01e8);
}

#[test]
fn earth_mars_synodic_period_is_780_days() {
    close(synodic_period(365.256 * DAY, 686.98 * DAY) / DAY, 779.9, 0.5);
}

// Earth to Mars from a 300 km parking orbit: v_inf 2.94 km/s needs 3.6 km/s.
#[test]
fn hyperbolic_departure_matches_textbook() {
    close(hyperbolic_burn(MU_EARTH, 6_678e3, 2_940.0), 3_590.0, 10.0);
}

#[test]
fn hohmann_is_reversible() {
    let up = Hohmann::new(MU_EARTH, 7e6, 4e7);
    let down = Hohmann::new(MU_EARTH, 4e7, 7e6);
    close(up.total(), down.total(), 1e-6);
    close(up.time_s, down.time_s, 1e-9);
}

#[test]
fn mars_lead_angle_is_44_degrees() {
    let n_mars = TAU / (686.98 * DAY);
    let h = Hohmann::new(1.327_124e20, 1.496e11, 2.279e11);
    close(lead_angle(n_mars, h.time_s).to_degrees(), 44.3, 0.5);
}

#[test]
fn home_moon_plan_is_bound_and_inside_hill_sphere() {
    let s = home();
    let (home, moon) = (s.id("home").unwrap(), s.id("moon").unwrap());
    let plan = s.transfer_plan(home, moon, 2_000.0, 200.0).unwrap();
    let r_park = s.radius(home) + 2_000.0;
    let mu_home = G * s.mass(home);
    // The cruise leg is an ellipse, so departing costs less than escaping.
    assert!(plan.depart < escape_speed(mu_home, r_park) - circular_speed(mu_home, r_park));
    assert!(plan.target_soi > s.radius(moon) + 200.0);
    let moon_el = s.elements(moon).unwrap();
    assert!(plan.target_soi < hill_radius(moon_el.a, 0.0, s.mass(moon), s.mass(home)));
    close(plan.plane_change.to_degrees(), 5.1, 1e-9);
    // The turn is folded into capture, so it costs more than a flat capture.
    let flat = hyperbolic_burn(G * s.mass(moon), s.radius(moon) + 200.0, plan.cruise.arrive.abs());
    assert!(plan.capture > flat);
}

#[test]
fn home_giant_window_lines_up_the_lead_angle() {
    let s = home();
    let (home, giant) = (s.id("home").unwrap(), s.id("giant").unwrap());
    let ticks_per_s = 1.0 / TICK;
    let synodic = synodic_period(11_520.0, 92_160.0) * ticks_per_s;
    let horizon = (synodic * 1.1) as u64;
    let first = s.next_window(home, giant, 0, 600, horizon).unwrap();
    let second = s.next_window(home, giant, first + 1, 600, horizon).unwrap();
    // Eccentric rails make windows wander around the circular synodic period.
    close((second - first) as f64 / synodic, 1.0, 0.05);
    let home_el = s.elements(home).unwrap();
    let giant_el = s.elements(giant).unwrap();
    let cruise = Hohmann::new(home_el.mu, home_el.a, giant_el.a);
    let lead = lead_angle(giant_el.mean_motion(), cruise.time_s);
    let phase = longitude(s.relative_state(giant, first).0)
        - longitude(s.relative_state(home, first).0);
    let miss = (phase - lead + PI).rem_euclid(TAU) - PI;
    assert!(miss.abs() < 1e-3, "phase misses the lead angle by {miss} rad");
}

#[test]
fn unrelated_bodies_have_no_plan() {
    let s = home();
    let (moon, giant) = (s.id("moon").unwrap(), s.id("giant").unwrap());
    assert!(s.transfer_plan(moon, giant, 100.0, 100.0).is_none());
    assert!(s.next_window(moon, giant, 0, 60, 1_000).is_none());
}
